// llg-test-fixture: tests/fixtures/sim/partial_features/udp_edge_rejected.sv
// IEEE 1364-2001 8.3; IEEE 1800-2009 29.5. Edge-sensitive table rows are
// rejected by the combinational UDP policy.
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
