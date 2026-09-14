// R09: derive expectations from a saved stream, not hardcoded RNG numbers.
module tb;
    process parent;
    string saved;
    int unsigned expected_seed, expected_child, expected_parent_next;
    int unsigned actual_child, actual_parent_next, ignored;
    initial begin
        parent = process::self();
        parent.srandom(32'h12345678);
        repeat (2) begin
            // This draw must affect the seed of the subsequently created child.
            ignored = $urandom;
            saved = parent.get_randstate();
            expected_seed = $urandom;
            expected_parent_next = $urandom;
            parent.srandom(expected_seed);
            expected_child = $urandom;
            parent.set_randstate(saved);
            fork
                actual_child = $urandom;
            join
            actual_parent_next = $urandom;
            if (actual_child !== expected_child ||
                actual_parent_next !== expected_parent_next)
                $fatal(0, "child did not consume exactly the next parent draw");
        end
        $display("child seeding ok");
        $finish(0);
    end
endmodule
