module tb;
    int calls;
    real a,nan_value,infinity;
    function automatic int next_value(); calls=calls+1; return calls; endfunction
    initial begin
        a=$pow(next_value(),2.0);
        $display("calls %0d %.1f",calls,a);
        a=-1.0;
        nan_value=$sqrt(a);
        infinity=$ln(0.0);
        $display("domain %b %b",nan_value!=nan_value,infinity < -1.0e300);
        $finish(0);
    end
endmodule
