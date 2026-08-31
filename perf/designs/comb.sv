// Copied verbatim from tests/sim_stress.rs `stress_wide_comb_tree`.
module tb;
    reg [63:0] a, b, c, d;
    reg [1:0] sel;
    reg [7:0] prio;
    reg [2:0] enc;
    wire [63:0] sum1, sum2, muxed;
    wire parity;

    assign sum1  = a + b;
    assign sum2  = sum1 + c;             // adder chain: reads sum1
    assign muxed = (sel == 2'd0) ? a :
                   (sel == 2'd1) ? b :
                   (sel == 2'd2) ? sum2 : d;
    assign parity = ^sum2;               // 64-bit reduction XOR

    always_comb begin
        casez (prio)
            8'b1???????: enc = 3'd7;
            8'b01??????: enc = 3'd6;
            8'b001?????: enc = 3'd5;
            8'b0001????: enc = 3'd4;
            8'b00001???: enc = 3'd3;
            8'b000001??: enc = 3'd2;
            8'b0000001?: enc = 3'd1;
            8'b00000001: enc = 3'd0;
            default: enc = 3'd0;
        endcase
    end

    initial begin
        a = 64'd1; b = 64'd2; c = 64'd3; d = 64'd4;
        sel = 2'd2; prio = 8'b0010_0000;
        #1 $display("t=1 sum1=%0d sum2=%0d muxed=%0d parity=%b enc=%0d", sum1, sum2, muxed, parity, enc);
        sel = 2'd3; a = 64'hFFFF_FFFF_FFFF_FFFF;
        #1 $display("t=2 sum1=%0d sum2=%0d muxed=%0d parity=%b enc=%0d", sum1, sum2, muxed, parity, enc);
        sel = 2'd0;
        #1 $display("t=3 sum1=%0d sum2=%0d muxed=%0d parity=%b", sum1, sum2, muxed, parity);
        prio = 8'b0000_0010;
        #1 $display("t=4 enc=%0d", enc);
        prio = 8'b1000_0000;
        #1 $display("t=5 enc=%0d", enc);
        prio = 8'b0000_0000;
        #1 $display("t=6 enc=%0d", enc);
        $finish;
    end
endmodule