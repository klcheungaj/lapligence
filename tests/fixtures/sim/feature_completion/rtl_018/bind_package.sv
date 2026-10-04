// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/bind_package.sv
// RTL-018 A03 negative (SV 2009 §23.11): a package is not a bind target scope.
module rtl018_obs; endmodule
package rtl018_pkg; endpackage
module tb; initial $finish; endmodule
bind rtl018_pkg rtl018_obs o();
