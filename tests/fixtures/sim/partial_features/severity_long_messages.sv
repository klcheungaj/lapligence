// llg-test-fixture: tests/fixtures/sim/partial_features/severity_long_messages.sv
// Runtime diagnostics longer than the reporter's inline buffer keep every
// byte and end in one newline: a 2001-character $warning, a labeled
// assertion failure and the $finish report.
module tb;
    string s;
    int ok;

    initial begin
        ok = 0;
        repeat (100) s = {s, "0123456789"};
        $warning("%s|%s", s, s);
        long_label_for_the_failing_immediate_assertion: assert (ok == 1);
        $finish;
    end
endmodule
