//! Selección múltiple de capas: Ctrl + clic suma o quita una capa y Mayús + clic elige todas las del medio, desde la última
//! pulsada (como en las listas de cualquier programa). La capa «actual» (`cl`) es siempre la última pulsada y manda en el resto
//! de la interfaz; la selección múltiple sirve para agrupar varias capas a la vez y, más adelante, para otras operaciones.

use super::*;

/// Calcula la selección después de pulsar la capa `i`. `sel` es la selección actual (con `cl` dentro), `anchor` la última capa
/// pulsada sin Mayús y `order` las capas tal como se ven en la lista (sin las ocultas por plegado). Devuelve la nueva
/// selección (ordenada), la capa actual y el ancla.
pub(super) fn next_selection(sel: &[usize], cl: usize, anchor: usize, i: usize, ctrl: bool, shift: bool, order: &[usize]) -> (Vec<usize>, usize, usize) {
    if shift {
        let a = order.iter().position(|&x| x == anchor).or_else(|| order.iter().position(|&x| x == cl));
        let b = order.iter().position(|&x| x == i);
        if let (Some(a), Some(b)) = (a, b) {
            let (lo, hi) = (a.min(b), a.max(b));
            let mut out: Vec<usize> = if ctrl { sel.to_vec() } else { Vec::new() };
            out.extend(order[lo..=hi].iter().copied());
            out.sort_unstable();
            out.dedup();
            return (out, i, anchor);
        }
    }
    if ctrl {
        let mut out: Vec<usize> = sel.to_vec();
        if let Some(p) = out.iter().position(|&x| x == i) {
            out.remove(p);
            // Si se quita la actual, la actual pasa a ser otra de las elegidas (o sigue siendo esta si no queda ninguna).
            let new_cl = if i == cl { out.last().copied().unwrap_or(i) } else { cl };
            if out.is_empty() {
                out.push(i);
            }
            return (out, new_cl, i);
        }
        out.push(i);
        out.sort_unstable();
        return (out, i, i);
    }
    (vec![i], i, i)
}

impl LeonApp {
    /// Las capas elegidas: las de la selección múltiple si incluye la actual; si no, solo la actual.
    pub(super) fn selected_layers(&self) -> Vec<usize> {
        let n = self.project.layers.len();
        let mut v: Vec<usize> = self.lsel.iter().copied().filter(|&i| i < n).collect();
        if !v.contains(&self.cl) {
            return vec![self.cl];
        }
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Pulsa la capa `i` de una lista (con Ctrl o Mayús, según lo que se tenga pulsado).
    pub(super) fn click_layer(&mut self, i: usize, ctrl: bool, shift: bool, order: &[usize]) {
        if i >= self.project.layers.len() {
            return;
        }
        let sel = self.selected_layers();
        let (new, cl, anchor) = next_selection(&sel, self.cl, self.lanchor, i, ctrl, shift, order);
        self.lsel = new;
        self.cl = cl;
        self.lanchor = anchor;
        self.sel = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDER: [usize; 6] = [0, 1, 2, 3, 4, 5];

    #[test]
    fn un_clic_sin_teclas_elige_solo_esa_capa() {
        assert_eq!(next_selection(&[1, 2], 2, 1, 4, false, false, &ORDER), (vec![4], 4, 4));
    }

    #[test]
    fn ctrl_suma_y_quita_capas_sueltas() {
        let (s, cl, an) = next_selection(&[1], 1, 1, 3, true, false, &ORDER);
        assert_eq!((s.as_slice(), cl, an), (&[1, 3][..], 3, 3));
        let (s, cl, _) = next_selection(&s, cl, an, 5, true, false, &ORDER);
        assert_eq!((s.as_slice(), cl), (&[1, 3, 5][..], 5));
        // Quitar una que no es la actual no cambia la actual; quitar la actual la pasa a otra elegida.
        let (s2, cl2, _) = next_selection(&s, 5, 5, 1, true, false, &ORDER);
        assert_eq!((s2.as_slice(), cl2), (&[3, 5][..], 5));
        let (s3, cl3, _) = next_selection(&s2, 5, 1, 5, true, false, &ORDER);
        assert_eq!((s3.as_slice(), cl3), (&[3][..], 3));
        // Quitar la última que queda la deja elegida: siempre hay una capa elegida.
        let (s4, cl4, _) = next_selection(&[3], 3, 3, 3, true, false, &ORDER);
        assert_eq!((s4.as_slice(), cl4), (&[3][..], 3));
    }

    #[test]
    fn mayus_elige_todas_las_del_medio_desde_la_ultima_pulsada() {
        let (s, cl, an) = next_selection(&[1], 1, 1, 4, false, true, &ORDER);
        assert_eq!((s.as_slice(), cl, an), (&[1, 2, 3, 4][..], 4, 1), "el ancla se queda donde estaba");
        // Otro Mayús + clic cambia el rango desde la misma ancla, hacia arriba.
        let (s, cl, _) = next_selection(&s, cl, an, 0, false, true, &ORDER);
        assert_eq!((s.as_slice(), cl), (&[0, 1][..], 0));
        // Con Ctrl + Mayús se suma el rango a lo que ya estaba elegido.
        let (s, _, _) = next_selection(&[0, 5], 5, 5, 3, true, true, &ORDER);
        assert_eq!(s, vec![0, 3, 4, 5]);
    }

    #[test]
    fn el_rango_salta_las_capas_que_no_se_ven_en_la_lista() {
        // La 2 y la 3 están dentro de una colección plegada y no salen en la lista.
        let order = [0, 1, 4, 5];
        let (s, _, _) = next_selection(&[1], 1, 1, 5, false, true, &order);
        assert_eq!(s, vec![1, 4, 5]);
    }
}