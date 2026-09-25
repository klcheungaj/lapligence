config choose;
    design work.tb;
    default liblist cells;
    cell selected_leaf use cells.low;
    instance tb.u use cells.high;
endconfig
