// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/formal_process_matrix.sv
// IEEE 1800-2009 §§9.2.2, 13.5, 23.2: each process family calls zero-time
// input/const-ref functions and output/inout/ref tasks on separate variables.
module tb;
    logic [7:0] source = 8'd7;
    logic clk = 1'b0;
    logic enable = 1'b0;

    int always_in, always_const, always_out, always_ref;
    int always_io = 10;
    int comb_in, comb_const, comb_out, comb_ref;
    int comb_io = 20;
    int latch_in, latch_const, latch_out, latch_ref;
    int latch_io = 30;
    int ff_in, ff_const, ff_out, ff_ref;
    int ff_io = 40;

    function automatic int read_input(input logic [7:0] value);
        return int'(value) + 1;
    endfunction

    function automatic int read_const(const ref logic [7:0] value);
        return int'(value) + 2;
    endfunction

    task automatic write_output(output int value);
        value = 3;
    endtask

    task automatic write_inout(inout int value);
        value = value + 4;
    endtask

    task automatic write_ref(ref int value);
        value = 5;
    endtask

    always @(posedge clk) begin
        always_in = read_input(source);
        always_const = read_const(source);
        write_output(always_out);
        write_inout(always_io);
        write_ref(always_ref);
    end

    always_comb begin
        comb_in = read_input(source);
        comb_const = read_const(source);
        write_output(comb_out);
        write_inout(comb_io);
        write_ref(comb_ref);
    end

    always_latch if (enable) begin
        latch_in = read_input(source);
        latch_const = read_const(source);
        write_output(latch_out);
        write_inout(latch_io);
        write_ref(latch_ref);
    end

    always_ff @(posedge clk) begin
        ff_in <= read_input(source);
        ff_const <= read_const(source);
        write_output(ff_out);
        write_inout(ff_io);
        write_ref(ff_ref);
    end

    initial begin
        #1 enable = 1'b1;
        #1 clk = 1'b1;
        #1;
        if (always_in !== 8 || always_const !== 9 || always_out !== 3 ||
            always_io !== 14 || always_ref !== 5)
            $fatal(1, "always formal/process row mismatch");
        if (comb_in !== 8 || comb_const !== 9 || comb_out !== 3 ||
            comb_io !== 24 || comb_ref !== 5)
            $fatal(1, "always_comb formal/process row mismatch");
        if (latch_in !== 8 || latch_const !== 9 || latch_out !== 3 ||
            latch_io !== 34 || latch_ref !== 5)
            $fatal(1, "always_latch formal/process row mismatch");
        if (ff_in !== 8 || ff_const !== 9 || ff_out !== 3 ||
            ff_io !== 44 || ff_ref !== 5)
            $fatal(1, "always_ff formal/process row mismatch");

        $display("always=%0d,%0d,%0d,%0d,%0d",
                 always_in, always_const, always_out, always_io, always_ref);
        $display("comb=%0d,%0d,%0d,%0d,%0d",
                 comb_in, comb_const, comb_out, comb_io, comb_ref);
        $display("latch=%0d,%0d,%0d,%0d,%0d",
                 latch_in, latch_const, latch_out, latch_io, latch_ref);
        $display("ff=%0d,%0d,%0d,%0d,%0d",
                 ff_in, ff_const, ff_out, ff_io, ff_ref);
        $finish(0);
    end
endmodule
