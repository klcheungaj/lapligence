// V 4.1.5-4.1.6 / SV 11.4: runtime signed and unsigned arithmetic at non-byte widths.
module tb;
    reg signed [64:0] s65, sb65;
    reg [64:0] u65, ub65;
    reg signed [32:0] s33, sb33;
    reg [32:0] u33, ub33;
    reg signed [6:0] s7, sb7;
    reg [6:0] u7, ub7;
    integer seed;
    initial begin
        if (!$value$plusargs("seed=%d", seed) || seed != 1) begin
            $display("seed must be 1");
            $finish(1);
        end
        s7 = seed[0] ? -7'sd5 : -7'sd4;
        sb7 = seed[0] ? 7'sd2 : 7'sd3;
        u7 = seed[0] ? 7'd5 : 7'd4;
        ub7 = seed[0] ? 7'd2 : 7'd3;
        $display("s7=%h,%h,%h,%h,%h,%h,%h,%h", +s7, -s7, s7+sb7, s7-sb7, s7*sb7, s7/sb7, s7%sb7, s7**sb7);
        $display("u7=%h,%h,%h,%h,%h,%h,%h,%h", +u7, -u7, u7+ub7, u7-ub7, u7*ub7, u7/ub7, u7%ub7, u7**ub7);
        sb7 = 0; ub7 = 0;
        $display("zero7=%h,%h,%h,%h", s7/sb7, s7%sb7, u7/ub7, u7%ub7);
        s7 = 7'bx; u7 = 7'bz;
        $display("xz7=%h,%h,%h,%h", s7+7'sd1, s7**7'sd2, u7*7'd2, u7/7'd2);
        s7 = 7'bz; u7 = 7'bx;
        $display("zx7=%h,%h,%h,%h", s7+7'sd1, s7**7'sd2, u7*7'd2, u7/7'd2);
        s33 = seed[0] ? -33'sd5 : -33'sd4;
        sb33 = seed[0] ? 33'sd2 : 33'sd3;
        u33 = seed[0] ? 33'd5 : 33'd4;
        ub33 = seed[0] ? 33'd2 : 33'd3;
        $display("s33=%h,%h,%h,%h,%h,%h,%h,%h", +s33, -s33, s33+sb33, s33-sb33, s33*sb33, s33/sb33, s33%sb33, s33**sb33);
        $display("u33=%h,%h,%h,%h,%h,%h,%h,%h", +u33, -u33, u33+ub33, u33-ub33, u33*ub33, u33/ub33, u33%ub33, u33**ub33);
        sb33 = 0; ub33 = 0;
        $display("zero33=%h,%h,%h,%h", s33/sb33, s33%sb33, u33/ub33, u33%ub33);
        s33 = 33'bx; u33 = 33'bz;
        $display("xz33=%h,%h,%h,%h", s33+33'sd1, s33**33'sd2, u33*33'd2, u33/33'd2);
        s33 = 33'bz; u33 = 33'bx;
        $display("zx33=%h,%h,%h,%h", s33+33'sd1, s33**33'sd2, u33*33'd2, u33/33'd2);
        s65 = seed[0] ? -65'sd5 : -65'sd4;
        sb65 = seed[0] ? 65'sd2 : 65'sd3;
        u65 = seed[0] ? 65'd5 : 65'd4;
        ub65 = seed[0] ? 65'd2 : 65'd3;
        $display("s65=%h,%h,%h,%h,%h,%h,%h,%h", +s65, -s65, s65+sb65, s65-sb65, s65*sb65, s65/sb65, s65%sb65, s65**sb65);
        $display("u65=%h,%h,%h,%h,%h,%h,%h,%h", +u65, -u65, u65+ub65, u65-ub65, u65*ub65, u65/ub65, u65%ub65, u65**ub65);
        sb65 = 0; ub65 = 0;
        $display("zero65=%h,%h,%h,%h", s65/sb65, s65%sb65, u65/ub65, u65%ub65);
        s65 = 65'bx; u65 = 65'bz;
        $display("xz65=%h,%h,%h,%h", s65+65'sd1, s65**65'sd2, u65*65'd2, u65/65'd2);
        s65 = 65'bz; u65 = 65'bx;
        $display("zx65=%h,%h,%h,%h", s65+65'sd1, s65**65'sd2, u65*65'd2, u65/65'd2);
        $finish(0);
    end
endmodule
