// llg-test-fixture: tests/fixtures/sim/partial_features/udp_edge_rejected.sv
// IEEE 1364-2001 8.1.6 Table 40; IEEE 1800-2009 29.3.6 Table 29-1. An
// edge-sensitive row is only legal in a sequential UDP; the frontend reports
// it at the row.
primitive udp_edge(out, data);
    output out;
    input data;
    table
        (01) : 1;
    endtable
endprimitive

module tb;
    reg data;
    wire out;
    udp_edge u_edge(out, data);
    initial begin
        data = 0;
        $finish(0);
    end
endmodule
