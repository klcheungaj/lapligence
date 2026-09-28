`ifndef LLG_CORPUS_N
`define LLG_CORPUS_N 128
`endif

`ifndef LLG_CORPUS_ITERS
`define LLG_CORPUS_ITERS 100
`endif

module testbench_tasks #(
    parameter integer N = `LLG_CORPUS_N,
    parameter integer ITERS = `LLG_CORPUS_ITERS
);
    logic tick = 0;
    logic [N-1:0] done = '0;
    integer control_checksum = 0;
    integer task_checksum = 0;
    mailbox #(integer) messages = new(1);
    semaphore keys = new(0);

    // Delay-only timing tasks take the emitted C-call path. The deepest call
    // keeps eight native C task activations live at a suspension.
    task automatic delay_1(inout integer checksum, input integer value);
        #0 checksum = checksum + value;
    endtask
    task automatic delay_2(inout integer checksum, input integer value);
        delay_1(checksum, value); #0 checksum = checksum + 2;
    endtask
    task automatic delay_3(inout integer checksum, input integer value);
        delay_2(checksum, value); #0 checksum = checksum + 3;
    endtask
    task automatic delay_4(inout integer checksum, input integer value);
        delay_3(checksum, value); #0 checksum = checksum + 4;
    endtask
    task automatic delay_5(inout integer checksum, input integer value);
        delay_4(checksum, value); #0 checksum = checksum + 5;
    endtask
    task automatic delay_6(inout integer checksum, input integer value);
        delay_5(checksum, value); #0 checksum = checksum + 6;
    endtask
    task automatic delay_7(inout integer checksum, input integer value);
        delay_6(checksum, value); #0 checksum = checksum + 7;
    endtask
    task automatic delay_8(inout integer checksum, input integer value);
        delay_7(checksum, value); #0 checksum = checksum + 8;
    endtask

    // Event controls force inline expansion. Nesting still covers depths 1–8,
    // but resume points and locals are placed directly in the caller.
    task automatic event_1(inout integer checksum, ref logic source,
                           input integer value);
        @(posedge source) checksum = checksum + value;
    endtask
    task automatic event_2(inout integer checksum, ref logic source,
                           input integer value);
        event_1(checksum, source, value); @(posedge source) checksum = checksum + 2;
    endtask
    task automatic event_3(inout integer checksum, ref logic source,
                           input integer value);
        event_2(checksum, source, value); @(posedge source) checksum = checksum + 3;
    endtask
    task automatic event_4(inout integer checksum, ref logic source,
                           input integer value);
        event_3(checksum, source, value); @(posedge source) checksum = checksum + 4;
    endtask
    task automatic event_5(inout integer checksum, ref logic source,
                           input integer value);
        event_4(checksum, source, value); @(posedge source) checksum = checksum + 5;
    endtask
    task automatic event_6(inout integer checksum, ref logic source,
                           input integer value);
        event_5(checksum, source, value); @(posedge source) checksum = checksum + 6;
    endtask
    task automatic event_7(inout integer checksum, ref logic source,
                           input integer value);
        event_6(checksum, source, value); @(posedge source) checksum = checksum + 7;
    endtask
    task automatic event_8(inout integer checksum, ref logic source,
                           input integer value);
        event_7(checksum, source, value); @(posedge source) checksum = checksum + 8;
    endtask

    genvar i;
    for (i = 0; i < N; i = i + 1) begin : workers
        initial begin
            automatic integer checksum = 0;
            automatic integer iteration;
            for (iteration = 0; iteration < ITERS; iteration = iteration + 1) begin
                case ((i + iteration) & 7)
                    0: delay_1(checksum, i + 1);
                    1: delay_2(checksum, i + 1);
                    2: delay_3(checksum, i + 1);
                    3: delay_4(checksum, i + 1);
                    4: delay_5(checksum, i + 1);
                    5: delay_6(checksum, i + 1);
                    6: delay_7(checksum, i + 1);
                    default: delay_8(checksum, i + 1);
                endcase
                case ((i + iteration) & 7)
                    0: event_1(checksum, tick, i + 1);
                    1: event_2(checksum, tick, i + 1);
                    2: event_3(checksum, tick, i + 1);
                    3: event_4(checksum, tick, i + 1);
                    4: event_5(checksum, tick, i + 1);
                    5: event_6(checksum, tick, i + 1);
                    6: event_7(checksum, tick, i + 1);
                    default: event_8(checksum, tick, i + 1);
                endcase
            end
            task_checksum = task_checksum + checksum;
            done[i] = (checksum != 0);
        end
    end

    always #1 tick = ~tick;

    // Keep the synchronization and cancellation shapes in one bounded
    // control process so increasing N measures task/process scaling rather
    // than multiplying container capacity.
    initial begin : controls
        messages.put(7);
        fork
            begin
                integer value;
                #0 messages.get(value);
                control_checksum = control_checksum + value;
            end
            begin
                messages.put(11);
                control_checksum = control_checksum + 11;
            end
        join

        fork
            begin keys.get(); control_checksum = control_checksum + 13; end
            begin #0 keys.put(); end
        join_any
        wait fork;

        fork
            begin #0 control_checksum = control_checksum + 17; end
            begin #1 control_checksum = control_checksum + 19; end
        join_none
        wait fork;

        fork : cancelled_group
            begin #100 control_checksum = -1; end
            begin #0 disable cancelled_group; end
        join
    end

    initial begin
        wait (&done);
        #2;
        $display("testbench_tasks n=%0d iters=%0d done=%0d task_checksum=%0d control=%0d",
                 N, ITERS, $countones(done), task_checksum, control_checksum);
        $finish(0);
    end
endmodule
