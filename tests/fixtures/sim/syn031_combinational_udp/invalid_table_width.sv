// llg-test-fixture: tests/fixtures/sim/syn031_combinational_udp/invalid_table_width.sv
// IEEE 1364-2001 8.1.4; IEEE 1800-2009 29.3: each row covers every input.
primitive invalid_width(out, a, b);
    output out;
    input a, b;
    table
        0 : 0;
    endtable
endprimitive
module tb;
    reg a, b;
    wire y;
    invalid_width u(y, a, b);
endmodule
