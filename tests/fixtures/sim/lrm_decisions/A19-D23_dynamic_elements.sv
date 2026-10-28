// Decision A19-D23: elements of dynamic arrays, queues and associative
// arrays through VPI. An element handle names its array and an index (or
// an associative key) and selects the element at each access: after the
// array changes, the handle reads whatever element is now at that index or
// key, and fails with vpiError while there is none. vpi_handle_by_index
// selects an associative element by an integral key that exists. The
// vpiIndex of an element with a string key is a vpiConstant of type
// vpiStringConst holding the key (its value the key's packed image). Elements and properties are
// written with vpiNoDelay only; a delay mode fails with vpiError.
// cbValueChange is refused on a dynamic array and on its elements.
//
// IEEE 1800-2009 37.17 detail 18 (SystemVerilog-1800-2009.txt
// L58780-L58781): "If a variable is an element of an array (the
// vpiArrayMember property is TRUE), the vpiIndex iterator shall return the
// indexing expressions that select that specific variable out of the
// array."; 38.36.1 (L64642-L64644): cbValueChange "Specifically excluded are
// class objects, dynamic arrays, strings, queues, and associative arrays.";
// 37.3.8 (L57569-L57570): "For a transient object, one may release its
// handle after use or expect that handle to be released and become invalid
// when the object ceases to exist." The text does not say whether an
// element handle follows its storage or its index when the array is
// resized or reordered; the selection by index or key, the string key
// constant, the refusals and the delay limit are llg choices.
//
// Build A19-D23_dynamic_elements.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d23_hold()` and `task $d23_check()` declared as
// system tasks).
module tb;
    int da[];
    int aa[int];
    string sa[string];
    initial begin
        da = '{10, 11, 12};
        aa[5] = 50;
        sa["k"] = "v";
        $d23_hold;
        da = '{20, 21};
        $d23_check;
        da = new[1](da);
        aa.delete(5);
        sa.delete("k");
        $d23_check;
        aa[5] = 55;
        sa["k"] = "w";
        $d23_check;
        $display("da.size=%0d aa[5]=%0d", da.size(), aa[5]);
    end
endmodule
