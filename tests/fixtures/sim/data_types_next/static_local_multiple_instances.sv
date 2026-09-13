// IEEE 1800-2009 6.21 and 13.4.2: static subprogram storage belongs to each
// module instance, so elaborated instances do not share a declaration object.
module child #(parameter integer P = 0)(output integer result);
    function integer instance_value();
        static integer captured = P;
        instance_value = captured;
    endfunction

    initial result = instance_value();
endmodule

module tb;
    integer first;
    integer second;
    child #(.P(11)) first_instance(first);
    child #(.P(22)) second_instance(second);

    initial begin
        #1;
        if (first !== 11 || second !== 22) begin
            $display("FAIL static_local_multiple_instances got=%0d,%0d", first, second);
            $finish;
        end
        $display("PASS static_local_multiple_instances");
        $finish;
    end
endmodule
