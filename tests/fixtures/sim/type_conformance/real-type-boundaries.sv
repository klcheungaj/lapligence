module tb;
    real r;
    realtime rt;
    shortreal sr;
    integer i;
    int j;
    logic signed [128:0] wide;
    initial begin
        if (r != 0.0 || rt != 0.0 || sr != 0.0) $display("FAIL defaults");
        r=2.5; rt=-2.5; i=r; j=rt;
        if (i !== 3 || j !== -3 || $rtoi(r) !== 2 || $rtoi(rt) !== -2) $display("FAIL rounding");
        sr=16777217.0;
        if (sr != 16777216.0 || real'(sr) != 16777216.0) $display("FAIL shortreal precision");
        wide='0; wide[100]=1;
        r=wide; wide=r;
        if (wide !== (129'd1 << 100)) $display("FAIL positive wide roundtrip");
        wide=-wide; rt=wide; wide=rt;
        if (wide !== -(129'sd1 << 100)) $display("FAIL negative wide roundtrip");
        r=-0.0; rt=0.0;
        if (r != rt || !r !== 1'b1 || !rt !== 1'b1) $display("FAIL zero");
        $display("PASS real boundaries"); $finish(0);
    end
endmodule
