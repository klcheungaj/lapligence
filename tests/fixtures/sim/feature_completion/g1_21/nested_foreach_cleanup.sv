// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_21/nested_foreach_cleanup.sv
// G1-21 control_nested_foreach_cleanup: nested loops with an omitted foreach
// dimension, continue/break and preserved iterator scope. IEEE 1800-2009 12.7.3.
module tb;
    int a [0:1][0:2][0:1];
    int acc;
    int ii;

    initial begin
        // Only dimensions 0 and 2 traverse; the omitted middle dimension is
        // never consumed and the body may index it explicitly.
        acc = 0;
        foreach (a[i, , k]) begin
            if (i == 0 && k == 0) continue;
            if (i == 1 && k == 1) break;
            acc = acc * 10 + (i + 1) * 10 + k;
        end
        $display("skip acc=%0d", acc);

        // Nested traversal with continue/break; every iterator keeps its
        // lexical per-loop value and the innermost break wins.
        ii = 0;
        foreach (a[i, j, k]) begin
            ii++;
            if (j == 1) continue;
            if (i == 1 && j == 2 && k == 1) break;
            acc = acc + (i + 1) * 100 + j * 10 + k;
        end
        $display("nest acc=%0d ii=%0d", acc, ii);
        $finish(0);
    end
endmodule
