module tb;
    real a,b,c,d;
    int negative;
    logic [64:0] wide;
    initial begin
        a=16.0; b=2.0; negative=-3;
        wide=65'h10000000000000000;
        $display("%.6f %.6f %.6f %.6f",$sqrt(a),$pow(a,b),$ln($exp(b)),$log10(100.0));
        $display("%.6f %.6f %.6f %.6f",$floor(-2.25),$ceil(-2.25),$pow(negative,b),$sqrt(wide));
        a=0.5; b=0.25;
        $display("%.6f %.6f %.6f",$sin($asin(a)),$cos($acos(a)),$tan($atan(b)));
        $display("%.6f %.6f",$atan2(0.0,-1.0),$hypot(3.0,4.0));
        $display("%.6f %.6f %.6f",$sinh($asinh(a)),$cosh($acosh(2.0)),$tanh($atanh(b)));
        a=16.0; b=2.0; c=0.5; d=-0.25;
        $display("direct %.6f %.6f %.6f %.6f %.6f",$ln(a),$log10(a),$exp(b),$sqrt(a),$pow(a,b));
        $display("trig %.6f %.6f %.6f %.6f %.6f %.6f",$sin(c),$cos(c),$tan(c),$asin(c),$acos(c),$atan(c));
        $display("pair %.6f %.6f",$atan2(d,c),$hypot(a,b));
        $display("hyper %.6f %.6f %.6f %.6f %.6f %.6f",$sinh(c),$cosh(c),$tanh(c),$asinh(c),$acosh(a),$atanh(c));
        c=-2.25;
        $display("round %.6f %.6f",$floor(c),$ceil(c));
        $finish(0);
    end
endmodule
