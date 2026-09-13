module tb;
    reg a = 0;
    wire y;
    logic sampled;
    not g(y, a);
    always_comb sampled = y;
    initial begin
        #1;
        $display("sampled=%b", sampled);
        a = 1;
        #1;
        $display("sampled=%b", sampled);
        a = 0;
        #1;
        $display("sampled=%b", sampled);
        $finish(0);
    end
endmodule
