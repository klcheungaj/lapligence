module tb;
    logic a, result;
    initial begin
        a = 1;
        result = 1'b0 &&& a matches 1'b1 ? 1'b1 : 1'b0;
        $display("must reject even an unreachable pattern: %b", result);
        $finish(0);
    end
endmodule
