module tb;
    task automatic run(input integer value);
        integer kept = value + 1;
        real fraction = 0.5;
        string text = "cell";
        begin
            integer inner = kept + 1;
            #1;
            $display("%s %0d %0.1f", text, inner, fraction);
            if (value == 1) return;
        end
        begin : skipped
            integer disabled = 99;
            disable skipped;
            $display("bad %0d", disabled);
        end
        $display("kept %0d", kept);
    endtask
    initial begin
        run(1);
        run(2);
        run(1);
        $finish(0);
    end
endmodule
