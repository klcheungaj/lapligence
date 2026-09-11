module tb;
    real r;
    shortreal s;
    bit [64:0] b;
    logic [64:0] v;
    initial begin
        v='1; v[63]=1'bx; v[32]=1'bz;
        r<=#2 2.5; s<=#2 16777217.0; b<=#2 v;
        v=0;
        $display("issued %0t",$time);
        #3 $display("%.1f %.1f %h",r,s,b);
        $finish;
    end
endmodule
