# SYN-035 PLA exclusion

`pla_unselected.sv` calls the Verilog PLA task `$async$and$array` (IEEE
1364-2001 §17.5; IEEE 1800-2009 §20.17). PLA tasks are unsupported by
design (ADV-032). The public simulator must report a blocking
diagnostic in both editions and optimizer modes, with no generated no-op model.
