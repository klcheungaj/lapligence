// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/static_return_ref_actual.sv
// IEEE 1800-2009 §§6.21 and 13.5.2: a hierarchical static function result
// variable is passed directly to a user-defined task's ref formal.
module tb;
    function static logic [7:0] result_value;
        return result_value;
    endfunction

    task automatic write_ref(ref logic [7:0] value);
        value = 8'h77;
    endtask

    initial begin
        write_ref(tb.result_value.result_value);
        if (result_value() !== 8'h77)
            $fatal(1, "hierarchical static return ref actual was not updated");
        $display("static-return-ref=passed");
        $finish;
    end
endmodule
