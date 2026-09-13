// llg-test-fixture: tests/fixtures/sim/process_semantics/always_comb_time_zero.sv
// IEEE 1800-2009 §9.2.2.2: always_comb executes at time zero and excludes
// variables written by the block, including a block-local temporary.
module tb;
    logic a;
    logic y;

    function automatic logic read_a();
        read_a = a;
    endfunction

    always_comb begin : comb_block
        logic local_value;
        local_value = read_a();
        y = local_value;
        y = y;
    end

    initial begin
        a = 1'b0;
        #0 $display("zero y=%b", y);
        a = 1'b1;
        #0 $display("one y=%b", y);
        $finish;
    end
endmodule
