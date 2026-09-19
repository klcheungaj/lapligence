// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_31/statement_select_mutation.sv
// IEEE 1800-2009 11.4.1-11.4.3: statement-position compound assignment and
// pre/post increment/decrement of a select or array element resolve the
// target once, call the RHS once and commit one blocking store.
module tb;
    logic [7:0] mem [0:3];
    logic [15:0] word;
    integer i;
    integer calls;

    function automatic integer f;
        begin
            calls = calls + 1;
            f = 3;
        end
    endfunction

    initial begin
        mem[0] = 8'd1;
        mem[1] = 8'd2;
        mem[2] = 8'd10;
        mem[3] = 8'd4;
        word = 16'h00f0;

        // `i++` is evaluated once: the write lands on mem[2] and i advances.
        i = 2;
        calls = 0;
        mem[i++] += f();
        $display("compound mem2=%0d i=%0d calls=%0d", mem[2], i, calls);

        i = 1;
        mem[i]++;
        $display("postinc mem1=%0d i=%0d", mem[1], i);

        i = 3;
        ++mem[i];
        $display("preinc mem3=%0d i=%0d", mem[3], i);

        // Part selects wrap at the selected width only.
        word[7:4] += 4'd3;
        $display("part word=%h", word);
        word[11:8] -= 4'd1;
        $display("part2 word=%h", word);

        i = 0;
        mem[i]--;
        $display("dec mem0=%0d i=%0d", mem[0], i);
        $finish(0);
    end
endmodule
