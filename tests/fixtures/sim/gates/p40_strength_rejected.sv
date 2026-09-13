// IEEE 1364-2001 7.1.2 and 7.9: drive strengths are retained as a distinct
// structural-driver feature and are not silently lowered by P40.
module tb;
    reg a;
    wire y;
    and (strong1, pull0) g(y, a, a);
    initial begin
        a = 1'b1;
        #1;
        if (y !== 1'b1) begin
            $display("FAIL gate_strength");
            $finish;
        end
        $display("PASS gate_strength");
        $finish;
    end
endmodule
