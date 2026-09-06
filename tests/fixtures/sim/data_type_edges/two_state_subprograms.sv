// IEEE 1800-2009 6.8, 6.11, and 13.5: two-state variables default to zero;
// subroutine formals, locals, and returns convert X/Z to zero at their boundary.
module tb #(parameter WIDTH = 128);
    logic [WIDTH-1:0] source;
    logic [WIDTH-1:0] expected;
    logic [WIDTH-1:0] observer;
    logic [WIDTH-1:0] task_observer;
    bit [WIDTH-1:0] module_bits;
    integer failed;

    function automatic bit [WIDTH-1:0] coerced_return(
        input logic [WIDTH-1:0] input_value
    );
        coerced_return = input_value;
    endfunction

    function automatic logic [WIDTH-1:0] coerced_formal(
        input bit [WIDTH-1:0] input_value
    );
        coerced_formal = input_value;
    endfunction

    function automatic bit [WIDTH-1:0] coerced_local(
        input logic [WIDTH-1:0] input_value
    );
        bit [WIDTH-1:0] local_value;
        begin
            local_value = input_value;
            coerced_local = local_value;
        end
    endfunction

    function automatic bit [WIDTH-1:0] default_local();
        bit [WIDTH-1:0] local_value;
        default_local = local_value;
    endfunction

    task automatic coerced_task(
        input bit [WIDTH-1:0] input_value,
        output bit [WIDTH-1:0] output_value
    );
        bit [WIDTH-1:0] local_value;
        begin
            local_value = input_value;
            output_value = local_value;
        end
    endtask

    task automatic default_task_output(output bit [WIDTH-1:0] output_value);
    endtask

    initial begin
        failed = 0;
        if (module_bits !== '0) begin
            $display("FAIL module-default WIDTH=%0d", WIDTH);
            failed = 1;
        end

        source = '0;
        source[0] = 1'b1;
        source[1] = 1'bx;
        source[2] = 1'bz;
        source[64] = 1'b1;
        source[65] = 1'bx;
        source[WIDTH-1 -: 8] = 8'b1xz0_10zx;
        expected = '0;
        expected[0] = 1'b1;
        expected[64] = 1'b1;
        expected[WIDTH-1 -: 8] = 8'h88;

        observer = coerced_return(source);
        if (!failed && observer !== expected) begin
            $display("FAIL function-return WIDTH=%0d", WIDTH);
            failed = 1;
        end
        observer = coerced_formal(source);
        if (!failed && observer !== expected) begin
            $display("FAIL function-formal WIDTH=%0d", WIDTH);
            failed = 1;
        end
        observer = coerced_local(source);
        if (!failed && observer !== expected) begin
            $display("FAIL function-local WIDTH=%0d", WIDTH);
            failed = 1;
        end
        observer = default_local();
        if (!failed && observer !== '0) begin
            $display("FAIL function-local-default WIDTH=%0d", WIDTH);
            failed = 1;
        end

        coerced_task(source, task_observer);
        if (!failed && task_observer !== expected) begin
            $display("FAIL task-boundaries WIDTH=%0d", WIDTH);
            failed = 1;
        end
        task_observer = '1;
        default_task_output(task_observer);
        if (!failed && task_observer !== '0) begin
            $display("FAIL task-output-default WIDTH=%0d", WIDTH);
            failed = 1;
        end

        module_bits <= source;
        #1;
        if (!failed && module_bits !== expected) begin
            $display("FAIL module-nba-coercion WIDTH=%0d", WIDTH);
            failed = 1;
        end

        module_bits = '1;
        module_bits[WIDTH-1 -: 8] <= source[WIDTH-1 -: 8];
        #1;
        expected = '1;
        expected[WIDTH-1 -: 8] = 8'h88;
        if (!failed && module_bits !== expected) begin
            $display("FAIL module-select-nba WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS two_state_subprograms WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
