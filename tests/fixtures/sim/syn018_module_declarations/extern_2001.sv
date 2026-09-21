// llg-test-fixture: tests/fixtures/sim/syn018_module_declarations/extern_2001.sv
// IEEE 1800-2009 §23.5 is outside the selected IEEE 1364-2001 profile; the
// single extern declaration is an edition-boundary rejection control.
extern module syn018_legacy (input a, output y);

module tb;
  wire a;
  wire y;
  syn018_legacy legacy(.a(a), .y(y));
endmodule
