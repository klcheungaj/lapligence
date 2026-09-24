// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/return_slot_formals.sv
// IEEE 1800-2009 §§6.21 and 13.5: a static function's implicit result
// variable can be supplied as an output, inout, or ref subroutine actual.
// The receiving function returns zero so each result reflects the actual's
// separate output copy-out or ref write.
module tb;
    function logic write_output(output logic value);
        value = 1'b1;
        write_output = 1'b0;
    endfunction

    function logic write_inout(inout logic value);
        value = 1'b1;
        write_inout = 1'b0;
    endfunction

    function automatic logic write_ref(ref logic value);
        value = 1'b1;
        write_ref = 1'b0;
    endfunction

    function logic output_result;
        logic ignored_return;
        output_result = 1'b0;
        ignored_return = write_output(output_result);
        if (ignored_return !== 1'b0) $fatal(1, "unexpected output helper return");
    endfunction

    function logic inout_result;
        logic ignored_return;
        inout_result = 1'b0;
        ignored_return = write_inout(inout_result);
        if (ignored_return !== 1'b0) $fatal(1, "unexpected inout helper return");
    endfunction

    function logic ref_result;
        logic ignored_return;
        ref_result = 1'b0;
        ignored_return = write_ref(ref_result);
        if (ignored_return !== 1'b0) $fatal(1, "unexpected ref helper return");
    endfunction

    initial begin
        $display("output=%b inout=%b ref=%b", output_result(), inout_result(), ref_result());
        $finish(0);
    end
endmodule
