// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/limit.sv
// SIM-047 step 3: $dumplimit stops value dumping, not the declarations: a
// limit below the header size still writes the whole header, then a comment
// (IEEE 1364-2001 18.1.5; S47-D8).
`timescale 1ns/1ns
module tb;
    logic [7:0] a;
    logic [7:0] b;
    initial begin
        a = 8'h00;
        b = 8'hff;
        $dumpfile("trace.vcd");
        $dumplimit(16);
        $dumpvars(0, tb);
        #1 a = 8'h01;
        #1 b = 8'h02;
        $strobe("%0t a=%h b=%h", $time, a, b);
        #1 $finish;
    end
endmodule
