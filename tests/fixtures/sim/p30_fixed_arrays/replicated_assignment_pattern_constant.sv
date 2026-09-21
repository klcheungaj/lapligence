// IEEE 1800-2009 10.9.1: a constant replicated assignment pattern may be
// folded while initializing a fixed unpacked array.
module tb;
    typedef logic [7:0] lane_t;
    lane_t folded [0:1] = '{2{8'h3d}};

    initial begin
        if (folded[0] !== 8'h3d || folded[1] !== 8'h3d) begin
            $display("FAIL replicated_assignment_pattern_constant");
            $finish;
        end
        $display("PASS replicated_assignment_pattern_constant");
        $finish(0);
    end
endmodule
