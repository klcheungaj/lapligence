module tb;
    reg a = 0;
    wire y;
    logic sampled;
    not g(y, a);
    always_comb sampled = y;
    initial begin
        sampled = 0;
        #1;
        $finish(0);
    end
endmodule
