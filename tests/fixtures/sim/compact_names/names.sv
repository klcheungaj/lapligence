module compact_leaf #(parameter integer ID = 1)(output integer v);
    integer \bus+index = ID;
    function integer \sum+index (input integer x);
        return x + \bus+index ;
    endfunction
    assign v = \sum+index (3);
endmodule

module compact_branch(output integer v);
    compact_leaf #(.ID(2)) b(v);
endmodule

class CompactNames;
    task leaf();
    endtask
    function integer leaf_desc_sites();
        return 23;
    endfunction
endclass

module tb;
    CompactNames object_value = new;
    task automatic leaf();
        #1;
    endtask
    function automatic integer leaf_desc_sites();
        return 19;
    endfunction
    wire signed [31:0] one, two;
    compact_leaf #(.ID(1)) \a.b (one);
    compact_branch a(two);
    integer a_b = 1;
    integer a__b = 2;
    integer \a+b = 3;
    integer aZdb = 4;
    integer cI_aZdb = 5;
    integer very_long_signal_name_abcdefghijklmnopqrstuvwxyz_abcdefghijklmnopqrstuvwxyz_abcdefghijklmnopqrstuvwxyz_abcdefghijklmnopqrstuvwxyz_abcdefghijklmnopqrstuvwxyz_abcdefghijklmnopqrstuvwxyz_abcdefghijklmnopqrstuvwxyz = 37;
    integer woke = 0;
    event \wake+up ;
    for (genvar i = 0; i < 3; ++i) begin : sites
        integer v = i + 11;
    end
    initial begin
        @(\wake+up );
        woke = 1;
    end
    initial begin
        leaf();
        object_value.leaf();
        #1;
        -> \wake+up ;
        #1;
        $display("scope %m");
        $display("children %0d %0d", one, two);
        $display("descriptor %0d", leaf_desc_sites());
        $display("class descriptor %0d", object_value.leaf_desc_sites());
        $display("names %0d", a_b + a__b + \a+b + aZdb + cI_aZdb);
        $display("generate %0d %0d", sites[0].v, sites[2].v);
        $display("long %0d wake %0d", very_long_signal_name_abcdefghijklmnopqrstuvwxyz_abcdefghijklmnopqrstuvwxyz_abcdefghijklmnopqrstuvwxyz_abcdefghijklmnopqrstuvwxyz_abcdefghijklmnopqrstuvwxyz_abcdefghijklmnopqrstuvwxyz_abcdefghijklmnopqrstuvwxyz, woke);
        $finish(0);
    end
endmodule
