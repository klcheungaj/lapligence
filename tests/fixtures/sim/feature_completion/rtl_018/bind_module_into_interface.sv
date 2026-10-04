// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/bind_module_into_interface.sv
// Adopted FND-002 witness neg_module_bind_interface (L-F03-09-01).
// SV2009 23.11, 25.3: an interface cannot contain a module instance, so a
// module bound into an interface target is rejected.
module C; endmodule interface I; endinterface
module tb; I i(); initial $finish; endmodule
bind I C c();
