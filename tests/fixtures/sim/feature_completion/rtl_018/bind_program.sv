// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/bind_program.sv
// RTL-018 A03 negative (SV 2009 §§23.11, 24.3): a module cannot be injected
// into a program, which may not contain module instances.
module rtl018_obs; endmodule
program rtl018_prog; endprogram
module tb; rtl018_prog p(); initial $finish; endmodule
bind rtl018_prog rtl018_obs o();
