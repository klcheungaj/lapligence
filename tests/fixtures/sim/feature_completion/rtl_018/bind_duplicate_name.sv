// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/bind_duplicate_name.sv
// RTL-018 A03 negative (SV 2009 §§3.13, 23.11): a module-type bind and an
// instance bind both inject `probe` into tb.t; bound instances share the
// target's name space, so the second name is a redefinition.
module rtl018_obs(input logic a); endmodule
module rtl018_target; logic a; endmodule
module tb; rtl018_target t(); initial $finish; endmodule
bind rtl018_target rtl018_obs probe(.a(a));
bind tb.t rtl018_obs probe(.a(a));
