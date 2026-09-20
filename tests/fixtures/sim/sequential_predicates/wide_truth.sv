module tb;
    logic signed [128:0] wide;
    logic [64:0] middle;
    logic [7:0] result;
    real fractional;
    initial begin
        wide = 'x; wide[128] = 1;
        middle = '0; middle[64] = 1;
        fractional = 0.25;
        result = wide &&& middle &&& fractional ? 8'ha5 : 8'ha6;
        if (result !== 8'ha5) $fatal(1, "full-width truth or real rounding");
        fractional = -0.0;
        result = wide &&& middle &&& fractional ? 8'ha5 : 8'ha6;
        if (result !== 8'ha6) $fatal(1, "negative zero");
        wide = 'z; middle = 0; fractional = 1.0;
        result = wide &&& middle &&& fractional ? 8'ha5 : 8'ha6;
        if (result !== 8'b101001xx) $fatal(1, "wide Z must terminate before zero");
        wide = 0; middle = 'x;
        result = wide &&& middle ? 8'ha5 : 8'ha6;
        if (result !== 8'ha6) $fatal(1, "wide false");
        $display("wide_truth=pass");
        $finish(0);
    end
endmodule
