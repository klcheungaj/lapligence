// llg-test-fixture: tests/fixtures/sim/syn016_elaboration/unit_provider.sv
// This file must precede unit_consumer.sv in a merged compilation unit.
localparam int UNIT_WIDTH = 5;
function automatic int unit_add(input int n);
    return n + UNIT_WIDTH;
endfunction
package unit_shared;
    parameter int SHARED_WIDTH = 3;
endpackage
