"""Offline trusted-manager contract; no launcher, guardian or authority provider.

Only the future credential-isolated manager may supply these observations.
Validating a message or identity tuple does not authenticate its sender or
acquire the descriptor/pidfd authority described by that message.
"""
from dataclasses import dataclass
from enum import Enum
from typing import Protocol


class Phase(str, Enum):
    RESERVED = 'reserved'
    ACTIVE = 'active'
    SEALED = 'sealed'
    COMMITTED = 'committed'
    RETAINED = 'retained'


@dataclass(frozen=True)
class Binding:
    session: str                 # unpredictable 128-bit nonce
    manager_epoch: str           # changes on every manager restart
    boot: str
    supervisor_identity: tuple  # pidfd-owned identity, never a PID alone
    subtree_identity: tuple     # pinned mount/device/inode identity
    quota_identity: tuple       # pinned filesystem/project/generation
    destination_identity: tuple
    staging_identity: tuple

    def __post_init__(self):
        for value in (self.session, self.manager_epoch):
            if len(value) != 32 or any(c not in '0123456789abcdef' for c in value):
                raise ValueError('invalid nonce')
        if not self.boot:
            raise ValueError('missing boot identity')
        for identity in (self.supervisor_identity, self.subtree_identity,
                         self.quota_identity, self.destination_identity,
                         self.staging_identity):
            if len(identity) < 3 or any(type(v) is not int or v <= 0 for v in identity):
                raise ValueError('incomplete pinned identity')
        if self.staging_identity == self.destination_identity:
            raise ValueError('staging is not the destination')


class Manager(Protocol):
    """Must authenticate peer credentials and hold actual authority handles.

    seal_and_observe closes launch/migration/IPC/descriptor writer ingress and
    keeps it closed until commit/retention. stop_and_observe does bounded
    cgroup-wide kill and recursive population verification. Neither may infer
    ownership from a pathname or numeric PID. No implementation ships here.
    """
    def seal_and_observe(self, binding: Binding) -> 'Observation': ...
    def stop_and_observe(self, binding: Binding) -> 'Observation': ...


@dataclass(frozen=True)
class Observation:
    binding: Binding
    sequence: int
    placed_before_exec: bool = False
    authority_fds_closed: bool = False
    distinct_credentials: bool = False
    guardian_alive: bool = False
    ingress_closed: bool = False
    recursively_empty: bool = False
    quota_unchanged: bool = False
    write_confinement: bool = False


class Contract:
    """Non-production transition validator, used by qualification fixtures.

    Rejects stale/replayed reports and permanently retains uncertainty. It
    deliberately cannot issue a filesystem publication operation.
    """
    def __init__(self, binding):
        self.binding = binding
        self.phase = Phase.RESERVED
        self.sequence = 0

    def retain(self):
        self.phase = Phase.RETAINED
        return False

    def _accept(self, observed):
        if (not isinstance(observed, Observation)
                or self.phase in (Phase.RETAINED, Phase.COMMITTED)
                or observed.binding != self.binding
                or type(observed.sequence) is not int
                or observed.sequence != self.sequence + 1):
            return self.retain()
        self.sequence = observed.sequence
        return True

    def launched(self, observed):
        if self.phase != Phase.RESERVED or not self._accept(observed):
            return self.retain()
        if not all(value is True for value in (
                observed.placed_before_exec, observed.authority_fds_closed,
                observed.distinct_credentials, observed.guardian_alive,
                observed.quota_unchanged, observed.write_confinement)):
            return self.retain()
        self.phase = Phase.ACTIVE
        return True

    def seal(self, manager: Manager):
        if self.phase != Phase.ACTIVE:
            return self.retain()
        try:
            observed = manager.seal_and_observe(self.binding)
        except (OSError, TimeoutError):
            return self.retain()
        if not self._accept(observed) or not all(value is True for value in (
                observed.guardian_alive, observed.ingress_closed,
                observed.recursively_empty, observed.quota_unchanged,
                observed.write_confinement)):
            return self.retain()
        self.phase = Phase.SEALED
        return True

    def commit(self, *, consent, before_deadline, cancelled, backend_success,
               relays_success, limits_valid, staging_valid, authority_live):
        if self.phase != Phase.SEALED or cancelled is not False or not all(
                value is True for value in (
                    consent, before_deadline, backend_success, relays_success,
                    limits_valid, staging_valid, authority_live)):
            return self.retain()
        self.phase = Phase.COMMITTED
        return True

    def supervisor_lost(self, manager: Manager):
        # Even successful cleanup after loss cannot authorize publication.
        self.retain()
        try:
            observed = manager.stop_and_observe(self.binding)
            return (isinstance(observed, Observation)
                    and observed.binding == self.binding
                    and type(observed.sequence) is int
                    and observed.sequence == self.sequence + 1
                    and observed.guardian_alive is True
                    and observed.ingress_closed is True
                    and observed.recursively_empty is True)
        except (OSError, TimeoutError):
            return False
