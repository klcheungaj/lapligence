integer unit_seed = 7;
function integer unit_value();
    unit_value = unit_seed;
endfunction

package p47_pkg;
    parameter integer OFFSET = 3;
    integer shared = OFFSET;
    integer initialized = shared + 1;

    function integer next(input integer delta);
        shared = shared + delta;
        next = shared;
    endfunction

    function integer initialized_value();
        initialized_value = initialized;
    endfunction

    task bump(output integer value);
        integer local_count = 0;
        local_count = local_count + 1;
        value = local_count;
    endtask
endpackage

package facade_pkg;
    import p47_pkg::initialized_value;
    export p47_pkg::initialized_value;
endpackage

module user #(parameter integer DELTA = 1)(
    output integer base,
    output integer result,
    output integer task_count
);
    integer shared;

    initial begin
        shared = 99;
        base = p47_pkg::initialized_value();
        result = p47_pkg::next(DELTA);
        p47_pkg::bump(task_count);
    end
endmodule

module importer(output integer result);
    import p47_pkg::*;
    initial result = initialized_value();
endmodule

module exporter(output integer result);
    initial result = facade_pkg::initialized_value();
endmodule

module tb;
    integer base0;
    integer base1;
    integer result0;
    integer result1;
    integer task_count0;
    integer task_count1;
    integer imported;
    integer exported;
    integer unit_read;
    wire [31:0] mirror;

    assign mirror = p47_pkg::shared;

    user #(1) u0(base0, result0, task_count0);
    user #(2) u1(base1, result1, task_count1);
    importer imported_user(imported);
    exporter exported_user(exported);

    initial begin
        unit_read = unit_value();
        #1 $display("pkg=%0d,%0d,%0d;%0d,%0d,%0d;import=%0d;export=%0d;unit=%0d;mirror=%0d",
                    base0, result0, task_count0,
                    base1, result1, task_count1, imported, exported, unit_read, mirror);
        p47_pkg::next(1);
        #0 $display("mirror-after=%0d", mirror);
        $finish;
    end
endmodule
