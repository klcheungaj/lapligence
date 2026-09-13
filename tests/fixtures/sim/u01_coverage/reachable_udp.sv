// llg-test-fixture: tests/fixtures/sim/u01_coverage/reachable_udp.sv
primitive mux2 (out, sel, a, b);
    output out;
    input sel, a, b;
    table
        0 ? 1 : 0 ;
        0 0 ? : 0 ;
        1 ? 0 : 1 ;
        1 1 ? : 1 ;
        x 0 0 : 0 ;
        x 1 1 : 1 ;
    endtable
endprimitive

module tb;
    reg sel, a, b;
    wire y;
    mux2 u(y, sel, a, b);
    initial begin
        sel = 0;
        a = 0;
        b = 1;
        $finish;
    end
endmodule
