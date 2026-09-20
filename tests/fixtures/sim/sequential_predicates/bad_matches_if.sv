module tb;
    logic a, b;
    initial begin
        a = 0; b = 1;
        if (a matches 1'b0 &&& b) $display("must not erase the pattern");
        $finish(0);
    end
endmodule
