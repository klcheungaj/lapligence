module tb;
    logic [7:0] formal_result;
    logic [7:0] return_result;

    task automatic fill_formal(output logic [7:0] value);
        value = 8'h00;
        value[0] = 1'b1;
        if (value !== 8'h01) $fatal(1, "formal element write");
        value[7:4] = 4'ha;
        if (value !== 8'ha1) $fatal(1, "formal slice write");
        {value[7:4], value[3:0]} = 8'hb4;
        if (value !== 8'hb4) $fatal(1, "formal concatenation write");
    endtask

    function logic [7:0] build_return;
        build_return = 8'h00;
        build_return[0] = 1'b1;
        if (build_return !== 8'h01) $fatal(1, "return element write");
        build_return[7:4] = 4'ha;
        if (build_return !== 8'ha1) $fatal(1, "return slice write");
        {build_return[7:4], build_return[3:0]} = 8'hb5;
        if (build_return !== 8'hb5) $fatal(1, "return concatenation write");
    endfunction

    initial begin
        static logic [7:0] saved = 8'h00;
        automatic logic [7:0] activation = 8'h00;
        saved[0] = 1'b1;
        if (saved !== 8'h01) $fatal(1, "static local element write");
        saved[7:4] = 4'ha;
        if (saved !== 8'ha1) $fatal(1, "static local slice write");
        {saved[7:4], saved[3:0]} = 8'hb6;
        if (saved !== 8'hb6) $fatal(1, "static local concatenation write");
        activation[0] = 1'b1;
        if (activation !== 8'h01) $fatal(1, "automatic local element write");
        activation[7:4] = 4'ha;
        if (activation !== 8'ha1) $fatal(1, "automatic local slice write");
        {activation[7:4], activation[3:0]} = 8'hb7;
        if (activation !== 8'hb7) $fatal(1, "automatic local concatenation write");
        fill_formal(formal_result);
        return_result = build_return();
        #1;
        if (saved !== 8'hb6 || activation !== 8'hb7 ||
            formal_result !== 8'hb4 || return_result !== 8'hb5)
            $fatal(1, "selected activation lvalue mismatch");
        $display("local=%h,%h formal=%h return=%h",
                 saved, activation, formal_result, return_result);
        $finish(0);
    end
endmodule
