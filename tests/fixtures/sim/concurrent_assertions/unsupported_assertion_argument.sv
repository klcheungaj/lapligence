// Post-2009 $assertcontrol must be rejected by the strict 2009 profile.
module tb;
    logic clk;

    check: assert property (@(posedge clk) 1'b1);

    initial begin
        clk = 1'b0;
        $assertcontrol(6);
    end
endmodule
