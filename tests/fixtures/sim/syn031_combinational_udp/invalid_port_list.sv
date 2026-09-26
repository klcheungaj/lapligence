// llg-test-fixture: tests/fixtures/sim/syn031_combinational_udp/invalid_port_list.sv
// IEEE 1364-2001 8.1.1-8.1.2; IEEE 1800-2009 29.3: each listed port is declared.
primitive invalid_ports(out, a, missing);
    output out;
    input a;
    table
        0 0 : 0;
    endtable
endprimitive
module tb;
    reg a, b;
    wire y;
    invalid_ports u(y, a, b);
endmodule
