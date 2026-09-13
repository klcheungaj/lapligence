// llg-test-fixture: tests/fixtures/sim/regression_81/nested_detached_disable.sv
// IEEE 1364-2001 section 11 / IEEE 1800-2009 section 9.6.2: disabling an
// active outer named block also cancels a retained descendant fork activation.
module tb;
    integer child;

    initial begin : outer
        begin : inner
            fork
                begin
                    #5;
                    child = child + 1;
                end
            join_none
        end
        #10;
        $display("WRONG: outer body resumed");
    end

    initial begin
        child = 0;
        #1 disable outer;
        #5;
        $display("child=%0d", child);
        $finish(0);
    end
endmodule
