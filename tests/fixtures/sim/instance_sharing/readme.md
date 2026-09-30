# Instance body sharing

`identities.sv` covers four module instances sharing a timing task and an event
worker, plus four generated processes. A top-level process triggers three distinct
events and disables only `u1.live` while it is waiting. The independent oracle
expects counts `1 0 1 1`, generated contribution slots resolving to `bits=1010`,
no late displays, and each original `%m` path. The
`sim_instance_sharing` suite checks exact stdout in both optimizer modes.
The expected generate-path spelling preserves the existing simulator output.
