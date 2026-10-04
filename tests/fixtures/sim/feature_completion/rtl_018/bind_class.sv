// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/bind_class.sv
// RTL-018 A03 negative (SV 2009 §23.11): a class is not a bind target scope.
module rtl018_obs; endmodule
class rtl018_class; endclass
module tb; initial $finish; endmodule
bind rtl018_class rtl018_obs o();
