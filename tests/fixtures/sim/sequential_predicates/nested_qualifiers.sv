module tb;
    logic a, b, c;
    int selected, nested, optional;
    initial begin
        for (int i = 0; i < 4; i++) begin
            a = i[1]; b = i[0]; c = 1;
            unique if (a &&& b) selected = 3;
            else if (a &&& !b) selected = 2;
            else if (!a &&& b) selected = 1;
            else selected = 0;
            if (selected != i) $fatal(1, "unique branch roles");
            priority if (a &&& b) selected = 3;
            else if (a &&& c) selected = 2;
            else if (b &&& c) selected = 1;
            else selected = 0;
            if (selected != i) $fatal(1, "priority branch roles");
            optional = 0;
            unique0 if (a &&& b) optional = 3;
            else if (a &&& !b) optional = 2;
            if (optional != (a ? i : 0)) $fatal(1, "unique0 branch roles");
            nested = a &&& b ? (c &&& a ? 3 : 9) : (a &&& c ? 2 : (b &&& c ? 1 : 0));
            if (nested != i) $fatal(1, "nested conditional predicate");
        end
        $display("nested_qualifiers=pass");
        $finish(0);
    end
endmodule
