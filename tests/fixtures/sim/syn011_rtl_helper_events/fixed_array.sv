// llg-test-fixture: tests/fixtures/sim/syn011_rtl_helper_events/fixed_array.sv
// IEEE 1800-2009 §§7.4.2, 7.6, 7.7, 7.12, 9.4.2 and 13.4: a finite
// fixed-array value formal and private reduction are legal zero-time helper
// computation.
module tb;
    typedef logic [7:0] array_t [0:2];
    array_t source;
    int changes;

    function automatic int reduce(input array_t values);
        array_t local_values;
        local_values = values;
        return local_values.sum() with (int'(item));
    endfunction

    always @(reduce(source))
        changes = changes + 1;

    initial begin
        source = '{8'd1, 8'd2, 8'd3};
        #1 source[0] = 8'd4;
        #1 $display("fixed_array changes=%0d value=%0d", changes, reduce(source));
        $finish(0);
    end
endmodule
