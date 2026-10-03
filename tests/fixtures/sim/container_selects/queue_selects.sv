// llg-test-fixture: tests/fixtures/sim/container_selects/queue_selects.sv
// Select writes of packed queue elements (IEEE 1800-2009 §7.10, §11.5.1).
// `$` names the last element; a select write at index `$+1` appends an
// element created from the default value, and an index past `$+1` is
// ignored with a warning (§7.10.1).
module tb;
    logic [129:0] q[$];
    int i;

    initial begin
        q.push_back('0);
        q.push_back({130{1'b1}});
        i = 1;
        q[0][129:64] = 66'h2_aaaa_bbbb_cccc_dddd;
        q[i][64] = 1'b0;
        q[i][3 -: 4] = 4'bxxxx;
        q[i][7 -: 4] = 4'bxz01;
        q[$][100 +: 8] = 8'h00;
        $display("q0=%h", q[0]);
        $display("q1=%h %b", q[1][129:8], q[1][7:0]);
        q[q.size()][0] = 1'b1;
        $display("n=%0d q2=%h low=%b", q.size(), q[2][129:4], q[2][3:0]);
        q[q.size() + 1][0] = 1'b1;
        $display("n=%0d", q.size());
        $display("reads=%h %b %b", q[0][129:126], q[i][64], q[i][7:4]);
        $finish;
    end
endmodule
