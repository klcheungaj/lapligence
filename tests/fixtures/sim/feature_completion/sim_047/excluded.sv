// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/excluded.sv
// SIM-047 A03: a scope dump covers packed and real storage only. Strings,
// class handles, events, dynamic, queue and associative arrays, automatic
// subroutine variables and block variables have no VCD encoding in llg and
// are left out (S47-D9). Objects are created and dropped every cycle, so the
// collector reclaims them while the dump runs; no registration names them.
`timescale 1ns/1ns
class Node;
    int value;
    Node next;
    function new(int v);
        value = v;
    endfunction
endclass

module tb;
    logic [3:0] count;
    real ratio;
    string label;
    Node head;
    int dyn[];
    int q[$];
    int aa[int];
    event tick;
    int total;

    function automatic int sum_list(Node n);
        int acc;
        acc = 0;
        while (n != null) begin
            acc += n.value;
            n = n.next;
        end
        return acc;
    endfunction

    initial begin
        count = 4'd0;
        ratio = 0.0;
        total = 0;
        $dumpfile("trace.vcd");
        $dumpvars(0, tb);
        repeat (4) begin : cycle
            Node fresh;
            #1 count = count + 4'd1;
            fresh = new(int'(count));
            fresh.next = head;
            head = fresh;
            if (count == 4'd2) head = null;
            label = $sformatf("c%0d", count);
            dyn = new[count];
            q.push_back(int'(count));
            aa[int'(count)] = int'(count) * 2;
            total = sum_list(head) + dyn.size() + q.size() + aa.num();
            ratio = real'(total) / 4.0;
            ->tick;
            $strobe("%0t count=%0d total=%0d ratio=%.2f label=%s", $time, count, total,
                    ratio, label);
        end
        #1 $finish;
    end
endmodule
