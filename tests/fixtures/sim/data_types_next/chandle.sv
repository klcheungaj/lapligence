// IEEE 1800-2009 6.14: chandles default to null and support only handle/null
// assignment, equality, Boolean tests, and subroutine passing/return.
module tb;
    chandle first;
    chandle second;

    function chandle pass_through(input chandle value);
        pass_through = value;
    endfunction

    initial begin
        if (first != null || first !== null) begin
            $display("FAIL chandle default_null");
            $finish;
        end
        if (first) begin
            $display("FAIL chandle default_boolean");
            $finish;
        end

        second = first;
        if (second != null || second !== first) begin
            $display("FAIL chandle assignment");
            $finish;
        end
        if (second) begin
            $display("FAIL chandle assignment_boolean");
            $finish;
        end
        first = null;
        second = pass_through(first);
        if (!(first == second) || !(first === null)) begin
            $display("FAIL chandle comparison_subroutine");
            $finish;
        end
        if (second) begin
            $display("FAIL chandle subroutine_boolean");
            $finish;
        end

        $display("PASS chandle");
        $finish;
    end
endmodule
