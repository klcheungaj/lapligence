// IEEE 1800-2009 10.9.1: a replicated assignment-pattern count is positive.
module tb;
    logic [7:0] values [0:1];

    initial begin
        values = '{0{8'h2a}};
        $finish(0);
    end
endmodule
