// IEEE 1800-2009 §§18.13-18.14: process-local random streams, state replay,
// inclusive range endpoints, and child stream derivation.
module tb;
    int first;
    int replay;
    int next;
    int range_value;
    string saved;
    int child_a;
    int child_b;
    int seed;
    int failed;

    initial begin
        process::self().srandom(32'h1234_5678);
        first = $urandom;
        process::self().srandom(32'h1234_5678);
        replay = $urandom;
        if (replay !== first) begin
            $display("FAIL reseed replay");
            failed = 1;
        end
        seed = 32'h7654_3210;
        first = $urandom(seed);
        replay = $urandom(seed);
        if (replay !== first) begin
            $display("FAIL urandom seed replay");
            failed = 1;
        end
        saved = process::self().get_randstate();
        next = $urandom;
        process::self().set_randstate(saved);
        replay = $urandom;
        if (replay !== next) begin
            $display("FAIL random state replay");
            failed = 1;
        end

        range_value = $urandom_range(7, 3);
        if (range_value < 3 || range_value > 7) begin
            $display("FAIL forward range=%0d", range_value);
            failed = 1;
        end
        range_value = $urandom_range(3, 7);
        if (range_value < 3 || range_value > 7) begin
            $display("FAIL reversed range=%0d", range_value);
            failed = 1;
        end
        range_value = $urandom_range(7);
        if (range_value < 0 || range_value > 7) begin
            $display("FAIL single-endpoint range=%0d", range_value);
            failed = 1;
        end
        if ($urandom_range(11, 11) !== 11) begin
            $display("FAIL equal range");
            failed = 1;
        end
        fork
            begin
                child_a = $urandom;
                child_a = child_a ^ $urandom;
            end
            begin
                child_b = $urandom;
                child_b = child_b ^ $urandom;
            end
        join
        if (child_a === child_b) begin
            $display("FAIL sibling streams unexpectedly identical");
            failed = 1;
        end

        if (failed) $display("random streams failed");
        else $display("random streams ok");
        $finish;
    end
endmodule
