// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/nested_control_flow.sv
// IEEE 1800-2009 §§9.4, 9.5 and 12: suspension within structured statements
// retains the selected branch and loop variables; break and continue still
// target the innermost source loop after a resume.
module tb;
    integer i;
    integer w;
    integer r;
    integer f;
    integer sum = 0;

    initial begin
        for (i = 0; i < 4; i++) begin
            #1;
            if (i == 1) continue;
            case (i)
                0: begin #1; sum += 10; end
                2: begin #1; sum += 20; end
                default: begin #1; sum += 30; break; end
            endcase
        end

        w = 0;
        while (w < 3) begin
            #1;
            w++;
            if (w == 1) continue;
            sum += w;
            if (w == 2) break;
        end

        r = 0;
        repeat (4) begin
            #1;
            r++;
            if (r == 2) continue;
            sum += r;
            if (r == 3) break;
        end

        f = 0;
        forever begin
            #1;
            f++;
            if (f == 1) continue;
            sum += 67;
            break;
        end

        if (i != 3 || w != 2 || r != 3 || f != 2 || sum != 133)
            $fatal(1, "control-flow state mismatch");
        $display("PASS nested_control_flow sum=%0d i=%0d w=%0d r=%0d f=%0d",
            sum, i, w, r, f);
        $finish(0);
    end
endmodule
