# P018 host and Watch phase mapping

| Trial phase | Host evidence | Watch evidence | Limits |
| --- | --- | --- | --- |
| W0 installation | None | None | Must be stated by operator and independently reviewed; CI is not installation. |
| W1 adapter/app/advertisement | Powered adapter and manager readiness; RegisterApplication and RegisterAdvertisement calls/replies; owned unregister calls/replies | None | RF needs an independent observer. |
| W2 scan/connect/discovery | Session creation is observed when the challenge/status request creates state | Explicit scan, discovery, connection, service and characteristic callbacks | Host does not identify the connecting endpoint. |
| W3 P006 exchange | Inbound frame count and synthetic completion | Challenge/result lengths, acknowledged frame writes and digest verification | CoreBluetooth write callback is Watch-side evidence; host has no matching peer-ACK callback. |
| W4 P009 rejection | Bounded signed transport writes; existing research decision report | Verified request, consent category, LA category and response write/ACK stages | Signature verdict is only reported by the existing transport result; no authorization follows. |
| W5 P009 approval | Existing signed research decision | Verified request, explicit consent/LA and response transfer | Must be a real Watch run. Simulated signatures never qualify hardware. |
| W6 cancel/expiry/cleanup | Fixed timeout/cancel/disconnect categories and owned unregister attempt/reply | Deadline, cancel, disconnect and cleanup callback | RF cessation requires independent observation. |

All durations are local monotonic measurements. The validator never subtracts host clock values from Watch clock values. A shared trial label only groups files. `authorization` remains `never_evaluated`.
